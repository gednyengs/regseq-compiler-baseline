//! Domain attribute sibling-table data shapes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{FieldIdx, ItemId, StringId, Symbol, VariantIdx};
use crate::index_vec::IndexVec;
use crate::interrupt::InterruptForm;
use crate::source::Span;

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
