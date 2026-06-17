//! Serial device-class construct IR (construct #7).
//!
//! The author writes a `#[rseq::driver]` struct tagged `#[rseq::device_class(serial)]`
//! and implements the framework-pure serial trait surface (GRILL D02/D03):
//! base `Serial` (C04), the optional `SerialRuntimeLineConfig` plane (C03; v1
//! builds only `line_format_apply`), and the optional `SerialInterruptDriven`
//! plane (C01: `irq_source_set` + `irq_cause_decode`). The construct's confirmed
//! contribution is the runtime per-source interrupt mask: `irq_source_set` is
//! **backend-called** — the compiler emits calls with `(SerialIrqSource,
//! IrqSourceState)` enum literals at each per-OS site (WHEN), while the author
//! owns only the mask-register bits (the match arms, WHICH BITS) (I01).
//!
//! This module is the owned/serializable model of that surface, populated by the
//! `regseq-driver` bridge's `serial` recognition and consumed by the three
//! backends' serial lowering. It replaces the deferred/NULL-api treatment of
//! `ClassTag::Serial` **additively** — non-serial deferred classes keep the
//! fallback.
//!
//! Op bodies are stored as `InstanceRef` (content-addressed), NOT raw
//! `InstanceId`, so they survive the Phase-4 (`F4-2`) instance sort — the same
//! discipline as `device_lifecycle::LifecycleOps` and `interrupt::IsrHandler`.
//!
//! The framework-type identities the backends need to mint/compare literals are
//! carried as `TyId` + variant indices ([`SerialTypeBindings`]). Both `TyId` and
//! `VariantIdx` are **stable across the F4 item sort**: `sort_items_by_f4`
//! permutes the `items` table (not the `TyInterner`) and remaps the `ItemId`s
//! *inside* `Ty` entries transparently, so a stored `TyId` keeps resolving to the
//! same nominal type. The driver-keyed map therefore needs only a key re-map (by
//! the driver-struct `ItemId`), exactly like `interrupt`/`device_lifecycle`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, VariantIdx};
use crate::instance_ref::InstanceRef;
use crate::ty::UintKind;

/// Sibling table on `RegseqModel` for recognized serial-class drivers, keyed by
/// the `#[rseq::driver]` struct's final `ItemId` (assigned at Phase 4, like
/// `interrupt`). Sparse via map absence. `#[serde(default)]` on the model field
/// keeps pre-construct-#7 models deserializable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialTables {
    /// Recognized per-driver serial surfaces, keyed by the driver-struct Item.
    #[serde(with = "ordered_serial_map")]
    pub drivers: BTreeMap<ItemId, DriverSerial>,
}

/// One recognized serial-class driver: its base data path, the optional
/// capability planes (by impl-presence), and the resolved framework-type
/// identities the backends need.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverSerial {
    /// Base `Serial` data path (C04). All four ops are required for a
    /// well-formed serial driver; a missing op is a frontend diagnostic (T04),
    /// but the IR carries `Option` so snapshot/early phases can represent a
    /// partial surface.
    pub base: SerialBaseOps,
    /// `SerialRuntimeLineConfig` plane (C03); `Some` iff the driver
    /// `impl SerialRuntimeLineConfig`. v1 carries only `line_format_apply`.
    pub line_config: Option<SerialLineConfigOps>,
    /// `SerialInterruptDriven` plane (C01); `Some` iff the driver
    /// `impl SerialInterruptDriven`. The coarse capability gate is impl-presence
    /// (taxonomy doc §10 D12).
    pub interrupt_driven: Option<SerialInterruptOps>,
    /// `SerialQueueControl` plane (C05); `Some` iff the driver
    /// `impl SerialQueueControl`. v1 carries only `tx_drain_query` (T09 INCLUDE);
    /// `fifo_purge_hw` is a deferred/capability-gated member (T04), absent here.
    #[serde(default)]
    pub queue_control: Option<SerialQueueControlOps>,
    /// Resolved framework-type identities + variant maps the backends use to
    /// mint/compare enum literals and build aggregate values (B1/F3). Populated
    /// by the bridge (T03); default-empty in snapshot/early phases.
    #[serde(default)]
    pub types: SerialTypeBindings,
}

/// The base `Serial` data path (C04). Each op's authored body, force-materialized
/// to a content-addressed `InstanceRef`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialBaseOps {
    /// `port_service_set(scope, on) -> prev` — enable/service the port at a scope.
    pub port_service_set: Option<InstanceRef>,
    /// `data_ready_query(dir) -> bool` — TX-space / RX-data readiness predicate.
    pub data_ready_query: Option<InstanceRef>,
    /// `data_unit_write(unit)` — emit one data unit to TX.
    pub data_unit_write: Option<InstanceRef>,
    /// `data_unit_read() -> RxDatum` — read one unit with inline per-byte errors.
    pub data_unit_read: Option<InstanceRef>,
    /// The pinned representation of the inline per-unit `RxError` bitset returned
    /// by `data_unit_read` (C04/I04). The O2-analog for the data path: it lets
    /// the backends DECODE the inline error bits and map them to OS-native error
    /// reporting (Linux tty flags; Zephyr `uart_err_check`) instead of dropping
    /// them. Default-pinned to the v1 vocab; the author `RX_ERR_*` consts must
    /// match these bit positions (same convention discipline as `CauseSetRepr`).
    #[serde(default)]
    pub rx_error: RxErrorRepr,
}

/// The `SerialRuntimeLineConfig` plane (C03). v1 carries only the procedural
/// `line_format_apply` (D04); the deferred members (`line_format_readback`,
/// `break_set`, `flow_auto_set`) are frontend-rejected (T04) and absent here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialLineConfigOps {
    /// `line_format_apply(LineFormat) -> ConfigResult` — procedural baud→divisor
    /// + register encode; commits requested config, does not read back (I03).
    pub line_format_apply: Option<InstanceRef>,
}

/// The `SerialQueueControl` plane (C05; T09 INCLUDE — see
/// `docs/discussions/serial-uart-management/T09-tx-drain-query-decision.md`). v1
/// carries only the read-only TX-drain completion predicate; the deferred
/// sibling `fifo_purge_hw` is frontend-rejected (T04) and absent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialQueueControlOps {
    /// `tx_drain_query() -> bool` — shift-register-inclusive TX-drain completion
    /// (PL011 reads FR.BUSY). REQUIRED for any serial driver targeting Linux
    /// (`uart_ops.tx_empty` is dereferenced unconditionally by `serial_core`) or
    /// Zephyr interrupt-driven (`uart_driver_api.irq_tx_complete`); no
    /// synthesized fallback (the fact is not derivable from the base
    /// `data_ready_query(TxSpace)` = FR.TXFF).
    pub tx_drain_query: Option<InstanceRef>,
}

/// The `SerialInterruptDriven` plane (C01). The per-source mask op + the cause
/// decode, plus the pinned `CauseSet` representation (O2).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialInterruptOps {
    /// `irq_source_set(source, state)` — BACKEND-CALLED per-source mask op
    /// (D03/I01). The compiler emits calls with enum literals at each per-OS
    /// site; this body holds the author's match-arm mask writes (WHICH BITS).
    pub irq_source_set: Option<InstanceRef>,
    /// `irq_cause_decode() -> CauseSet` — decode pending serviceable causes. The
    /// authored body also W1C-acks the line-error causes as it decodes (there is
    /// no separate ack op; F1/B5), so error causes do not re-latch.
    pub irq_cause_decode: Option<InstanceRef>,
    /// The pinned `CauseSet` representation (GRILL O2).
    pub cause_set: CauseSetRepr,
}

/// O2 — the `CauseSet` representation. A bitset over the ordered serviceable
/// causes; the bit position of a cause is its [`SerialCause`] discriminant.
/// `modem_delta` is **present-but-unarmed** in v1 (no `SerialIrqSource` arms it;
/// it routes to `SerialModemControl` later).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CauseSetRepr {
    /// Backing scalar width. `U32` in v1 (PL011 `MIS`/`RIS` are 32-bit).
    pub width: UintKind,
    /// The ordered causes carried in the bitset (bit position = vector index =
    /// [`SerialCause`] discriminant).
    pub causes: Vec<CauseEntry>,
}

impl Default for CauseSetRepr {
    fn default() -> Self {
        CauseSetRepr {
            width: UintKind::U32,
            causes: SerialCause::V1_ORDER
                .iter()
                .map(|&cause| CauseEntry {
                    cause,
                    armable: cause.armable_v1(),
                })
                .collect(),
        }
    }
}

/// One cause carried in a [`CauseSetRepr`] bitset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CauseEntry {
    /// Which serviceable cause this bit denotes.
    pub cause: SerialCause,
    /// Whether v1 arms this cause through a `SerialIrqSource`. `false` for
    /// `modem_delta` (present-but-unarmed) — the one cause with no v1 source.
    pub armable: bool,
}

/// The serviceable interrupt causes a `CauseSet` can carry (O2). The
/// discriminant is the canonical bit position. Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SerialCause {
    /// TX path can accept more data (`SerialIrqSource::TxReady`).
    TxReady = 0,
    /// RX data available (`SerialIrqSource::RxData`).
    RxData = 1,
    /// RX idle-timeout with residual data (couples to `RxData`, Zephyr Z11).
    RxTimeout = 2,
    /// Line error (framing/parity/overrun/break) (`SerialIrqSource::LineError`).
    LineError = 3,
    /// Modem-status change. Present-but-unarmed in v1 (routes to
    /// `SerialModemControl`, not `SerialInterruptDriven`).
    ModemDelta = 4,
}

impl SerialCause {
    /// The v1 cause order; the index here is the canonical bit position.
    pub const V1_ORDER: [SerialCause; 5] = [
        SerialCause::TxReady,
        SerialCause::RxData,
        SerialCause::RxTimeout,
        SerialCause::LineError,
        SerialCause::ModemDelta,
    ];

    /// Whether v1 arms this cause via a `SerialIrqSource` (everything except
    /// `modem_delta`).
    pub fn armable_v1(self) -> bool {
        !matches!(self, SerialCause::ModemDelta)
    }

    /// The `SerialIrqSource` that arms this cause in v1, if any. `rx_timeout`
    /// couples to `RxData` (Zephyr Z11); `modem_delta` has no v1 source.
    pub fn arming_source(self) -> Option<IrqSourceKind> {
        match self {
            SerialCause::TxReady => Some(IrqSourceKind::TxReady),
            SerialCause::RxData | SerialCause::RxTimeout => Some(IrqSourceKind::RxData),
            SerialCause::LineError => Some(IrqSourceKind::LineError),
            SerialCause::ModemDelta => None,
        }
    }
}

/// The `RxError` representation (the data-path O2-analog). A bitset over the
/// ordered inline RX error kinds returned by `data_unit_read`; the bit position
/// of a kind is its [`SerialRxErrorKind`] discriminant. The backends decode
/// `RxDatum.errors` against this and map to OS-native error reporting (Linux
/// `TTY_PARITY`/`TTY_FRAME`/`TTY_OVERRUN`/`TTY_BREAK`; Zephyr
/// `UART_ERROR_*`/`UART_BREAK` via `uart_err_check`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RxErrorRepr {
    /// Backing scalar width. `U32` in v1 (the author `RxError` newtype is u32).
    pub width: UintKind,
    /// The ordered RX error kinds carried in the bitset (bit position = vector
    /// index = [`SerialRxErrorKind`] discriminant).
    pub kinds: Vec<SerialRxErrorKind>,
}

impl Default for RxErrorRepr {
    fn default() -> Self {
        RxErrorRepr {
            width: UintKind::U32,
            kinds: SerialRxErrorKind::V1_ORDER.to_vec(),
        }
    }
}

/// The inline per-unit RX error kinds (C04/I04). The discriminant is the
/// canonical bit position; matches the author `RX_ERR_*` consts. Closed,
/// append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SerialRxErrorKind {
    /// Framing error (`RX_ERR_FRAMING`).
    Framing = 0,
    /// Parity error (`RX_ERR_PARITY`).
    Parity = 1,
    /// Hardware FIFO overrun (`RX_ERR_HW_OVERRUN`).
    HwOverrun = 2,
    /// Software/buffer overrun (`RX_ERR_SW_OVERRUN`).
    SwOverrun = 3,
    /// Break condition (`RX_ERR_BREAK`).
    Break = 4,
}

impl SerialRxErrorKind {
    /// The v1 kind order; the index here is the canonical bit position.
    pub const V1_ORDER: [SerialRxErrorKind; 5] = [
        SerialRxErrorKind::Framing,
        SerialRxErrorKind::Parity,
        SerialRxErrorKind::HwOverrun,
        SerialRxErrorKind::SwOverrun,
        SerialRxErrorKind::Break,
    ];
}

/// The canonical `SerialIrqSource` semantics (C01). The bridge validates the
/// authored enum's variants against these and records the per-semantic variant
/// index in [`SerialTypeBindings::irq_source`].
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IrqSourceKind {
    /// TX-ready source.
    TxReady = 0,
    /// RX-data source (couples rx_timeout).
    RxData = 1,
    /// Line-error source.
    LineError = 2,
}

impl IrqSourceKind {
    /// Canonical order (index = canonical position in [`EnumBinding::variants`]).
    pub const ORDER: [IrqSourceKind; 3] = [
        IrqSourceKind::TxReady,
        IrqSourceKind::RxData,
        IrqSourceKind::LineError,
    ];
}

/// The canonical `IrqSourceState` semantics (C01).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IrqStateKind {
    /// Source armed (unmasked).
    Armed = 0,
    /// Source disarmed (masked).
    Disarmed = 1,
}

impl IrqStateKind {
    /// Canonical order (index = canonical position in [`EnumBinding::variants`]).
    pub const ORDER: [IrqStateKind; 2] = [IrqStateKind::Armed, IrqStateKind::Disarmed];
}

/// Normalized variant-index maps for the support enums the backends mint or
/// compare as literals (I02 — "the compiler normalizes enum literals to
/// placement intents"; F3). The bridge captures each map from the recognized
/// trait method signatures' rustc `AdtDef`s and validates the variant names
/// against the closed vocab (T04). Backends read these maps and recover the
/// enum/struct `TyId`s from the op function signatures (InstanceRef → FnSig),
/// and read variant/field names from the type layout — so this side table holds
/// only the semantic→variant-index normalization (the spine; C-2 "backends read
/// IR, never the AST"). `VariantIdx` is a source-order fact, stable across the
/// F4 item sort. Populated by the bridge (T03); each field is `None` until
/// its plane is recognized.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialTypeBindings {
    /// `SerialIrqSource {TxReady,RxData,LineError}` — variants in
    /// [`IrqSourceKind::ORDER`].
    pub irq_source: Option<EnumBinding>,
    /// `IrqSourceState {Armed,Disarmed}` — variants in [`IrqStateKind::ORDER`].
    pub irq_state: Option<EnumBinding>,
    /// `ConfigResult {Ok,Unsupported}` — variants `[Ok, Unsupported]` (I03).
    pub config_result: Option<EnumBinding>,
    /// `PortScope {Block,Tx,Rx}` — variants `[Block, Tx, Rx]`.
    pub port_scope: Option<EnumBinding>,
    /// `ReadyDir {TxSpace,RxData}` — variants `[TxSpace, RxData]`.
    pub ready_dir: Option<EnumBinding>,
    /// `DataBits {Five,Six,Seven,Eight,Nine}` — variants in that order.
    pub data_bits: Option<EnumBinding>,
    /// `Parity {None,Even,Odd}` — variants in that order.
    pub parity: Option<EnumBinding>,
    /// `StopBits {One,Two}` — variants `[One, Two]`.
    pub stop_bits: Option<EnumBinding>,
}

/// A recognized support enum: the authored variant index for each canonical
/// semantic position. The bridge validates the authored variant names against
/// the closed vocab and fills `variants` in the canonical order documented on
/// the owning [`SerialTypeBindings`] field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumBinding {
    /// `variants[i]` = the authored variant index for canonical semantic
    /// position `i` (e.g. `SerialIrqSource`: `[TxReady, RxData, LineError]`).
    pub variants: Vec<VariantIdx>,
}

/// Serde adapter: serialize the `ItemId`-keyed driver map as a JSON array of
/// `(key, value)` pairs (mirrors `interrupt::ordered_interrupt_map`).
/// `BTreeMap` iteration is key-sorted, so output is deterministic across runs.
mod ordered_serial_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        map: &BTreeMap<ItemId, DriverSerial>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<(&ItemId, &DriverSerial)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<ItemId, DriverSerial>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(ItemId, DriverSerial)>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args_key::ArgsKey;
    use crate::crate_info::StableCrateId;
    use crate::def_path::DefPathHash;
    use serde::de::DeserializeOwned;

    fn iref(n: u32) -> InstanceRef {
        InstanceRef {
            regseq_kind_rank: 0,
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(n as u64),
                local_hash: n as u64,
            },
            args_key: ArgsKey(n as u128),
            fn_shim_disambig: 0,
        }
    }
    fn vidx(n: u32) -> VariantIdx {
        VariantIdx::from_u32(n)
    }

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
        let json = serde_json::to_string(value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&back, value);
    }

    fn sample_driver() -> DriverSerial {
        DriverSerial {
            base: SerialBaseOps {
                port_service_set: Some(iref(1)),
                data_ready_query: Some(iref(2)),
                data_unit_write: Some(iref(3)),
                data_unit_read: Some(iref(4)),
                rx_error: RxErrorRepr::default(),
            },
            line_config: Some(SerialLineConfigOps {
                line_format_apply: Some(iref(5)),
            }),
            interrupt_driven: Some(SerialInterruptOps {
                irq_source_set: Some(iref(6)),
                irq_cause_decode: Some(iref(7)),
                cause_set: CauseSetRepr::default(),
            }),
            queue_control: Some(SerialQueueControlOps {
                tx_drain_query: Some(iref(8)),
            }),
            types: SerialTypeBindings {
                irq_source: Some(EnumBinding {
                    variants: vec![vidx(0), vidx(1), vidx(2)],
                }),
                irq_state: Some(EnumBinding {
                    variants: vec![vidx(0), vidx(1)],
                }),
                config_result: Some(EnumBinding {
                    variants: vec![vidx(0), vidx(1)],
                }),
                port_scope: Some(EnumBinding {
                    variants: vec![vidx(0), vidx(1), vidx(2)],
                }),
                ready_dir: Some(EnumBinding {
                    variants: vec![vidx(0), vidx(1)],
                }),
                data_bits: Some(EnumBinding {
                    variants: vec![vidx(0), vidx(1), vidx(2), vidx(3), vidx(4)],
                }),
                parity: Some(EnumBinding {
                    variants: vec![vidx(0), vidx(1), vidx(2)],
                }),
                stop_bits: Some(EnumBinding {
                    variants: vec![vidx(0), vidx(1)],
                }),
            },
        }
    }

    fn sample_tables() -> SerialTables {
        let mut tables = SerialTables::default();
        tables.drivers.insert(ItemId::from_u32(3), sample_driver());
        tables
    }

    #[test]
    fn serial_tables_round_trip() {
        round_trip(&sample_tables());
    }

    #[test]
    fn driver_serial_round_trip() {
        round_trip(&sample_driver());
    }

    #[test]
    fn serial_map_json_is_deterministic_across_insert_order() {
        let mut a = SerialTables::default();
        let mut b = SerialTables::default();
        a.drivers.insert(ItemId::from_u32(1), DriverSerial::default());
        a.drivers.insert(ItemId::from_u32(2), sample_driver());
        b.drivers.insert(ItemId::from_u32(2), sample_driver());
        b.drivers.insert(ItemId::from_u32(1), DriverSerial::default());
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn cause_set_default_pins_o2() {
        // O2: the default CauseSet is a u32 bitset over the five v1 causes, in
        // canonical bit order, with modem_delta present-but-unarmed.
        let cs = CauseSetRepr::default();
        assert_eq!(cs.width, UintKind::U32);
        assert_eq!(cs.causes.len(), 5);
        let causes: Vec<SerialCause> = cs.causes.iter().map(|c| c.cause).collect();
        assert_eq!(causes, SerialCause::V1_ORDER.to_vec());
        // bit position == discriminant
        for (bit, entry) in cs.causes.iter().enumerate() {
            assert_eq!(bit as u8, entry.cause as u8);
        }
        // modem_delta is the sole present-but-unarmed cause
        for entry in &cs.causes {
            if entry.cause == SerialCause::ModemDelta {
                assert!(!entry.armable);
            } else {
                assert!(entry.armable);
            }
        }
    }

    #[test]
    fn rx_error_default_pins_the_v1_vocab() {
        // The data-path O2-analog: a u32 bitset over the five inline RX error
        // kinds, bit position = SerialRxErrorKind discriminant, matching the
        // author RX_ERR_* consts (Framing=0,Parity=1,HwOverrun=2,SwOverrun=3,
        // Break=4).
        let r = RxErrorRepr::default();
        assert_eq!(r.width, UintKind::U32);
        assert_eq!(r.kinds, SerialRxErrorKind::V1_ORDER.to_vec());
        for (bit, kind) in r.kinds.iter().enumerate() {
            assert_eq!(bit as u8, *kind as u8);
        }
    }

    #[test]
    fn cause_arming_source_couples_rx_timeout_to_rxdata() {
        assert_eq!(
            SerialCause::TxReady.arming_source(),
            Some(IrqSourceKind::TxReady)
        );
        assert_eq!(
            SerialCause::RxData.arming_source(),
            Some(IrqSourceKind::RxData)
        );
        assert_eq!(
            SerialCause::RxTimeout.arming_source(),
            Some(IrqSourceKind::RxData)
        );
        assert_eq!(
            SerialCause::LineError.arming_source(),
            Some(IrqSourceKind::LineError)
        );
        assert_eq!(SerialCause::ModemDelta.arming_source(), None);
    }

    #[test]
    fn empty_driver_default_has_no_planes() {
        let d = DriverSerial::default();
        assert!(d.line_config.is_none());
        assert!(d.interrupt_driven.is_none());
        assert!(d.queue_control.is_none());
        assert!(d.base.port_service_set.is_none());
        assert_eq!(d.types, SerialTypeBindings::default());
    }

    #[test]
    fn driver_without_types_field_deserializes_to_default() {
        // A driver serialized before SerialTypeBindings existed (or by an early
        // recognition phase) must deserialize with the default-empty bindings.
        let json = r#"{"base":{"port_service_set":null,"data_ready_query":null,"data_unit_write":null,"data_unit_read":null},"line_config":null,"interrupt_driven":null}"#;
        let d: DriverSerial = serde_json::from_str(json).expect("deserialize");
        assert_eq!(d.types, SerialTypeBindings::default());
    }
}
