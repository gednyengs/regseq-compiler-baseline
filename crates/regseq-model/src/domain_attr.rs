//! Domain attribute sibling-table data shapes.

use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

use crate::ids::{FieldIdx, ItemId, Symbol, VariantIdx};
use crate::index_vec::IndexVec;
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
    #[serde(with = "hash_map_vec")]
    pub field_attrs: HashMap<FieldPath, Vec<AttrData>>,

    /// Attributes attached to source-code spans.
    ///
    /// This table is reserved for future diagnostic annotations and
    /// lint-style markers. It is sparse via absence in the map.
    #[serde(with = "hash_map_vec")]
    pub span_attrs: HashMap<Span, Vec<AttrData>>,
}

/// Composite key for the field-attribute sibling table.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
            _ => None,
        }
    }
}

mod hash_map_vec {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<K, V, S>(map: &HashMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        K: Serialize + Eq + Hash,
        V: Serialize,
        S: Serializer,
    {
        let entries: Vec<(&K, &V)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, K, V, D>(deserializer: D) -> Result<HashMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + Eq + Hash,
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
}
