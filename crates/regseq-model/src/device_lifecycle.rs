//! Device-lifecycle construct IR (construct #2).
//!
//! The author writes a `#[rseq::driver]` struct, an optional
//! `#[rseq::device_class]` tag, and an optional `Lifecycle` trait impl. This
//! module is the owned/serializable model of that surface: the driver
//! descriptor (name + typed match set + device class), the five optional
//! lifecycle callbacks, and the driver handle (resource vs non-resource
//! user-data fields). It is populated by the `regseq-driver` bridge's
//! `device_lifecycle` recognition and consumed by the `generic_c` backend's
//! lifecycle glue.
//!
//! Callback bodies are stored as `InstanceRef` (content-addressed), NOT raw
//! `InstanceId`, so they survive the Phase-4 (`F4-2`) instance sort that
//! reorders `model.instances`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, StringId, TyId};
use crate::instance_ref::InstanceRef;

/// Sibling table on `RegseqModel` for recognized device-lifecycle drivers,
/// keyed by the `#[rseq::driver]` struct's final `ItemId` (assigned at
/// Phase 4, like `domain_attrs`). Sparse via map absence.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceLifecycleTables {
    /// Recognized drivers, keyed by the driver-struct Item.
    #[serde(with = "ordered_driver_map")]
    pub drivers: BTreeMap<ItemId, DriverLifecycle>,
}

/// One recognized device-lifecycle driver: its descriptor, handle, and the
/// resolved lifecycle callbacks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverLifecycle {
    /// The authored descriptor (name, typed match set, device class).
    pub descriptor: DriverDescriptor,
    /// The spec-view driver-struct fields (resource vs user-data).
    pub handle: DriverHandle,
    /// The five optional lifecycle callbacks (override-checked).
    pub ops: LifecycleOps,
}

/// The authored driver descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverDescriptor {
    /// Stable logical driver name (registration, symbols, diagnostics).
    pub name: StringId,
    /// Typed match set; non-empty for a well-formed driver (else diagnostic).
    pub match_set: Vec<MatchEntry>,
    /// Device-class tag (default `Generic`); stored, unvalidated this stage.
    pub device_class: ClassTag,
}

/// A typed device-match entry. All bus variants are modeled; the bridge parses
/// per-bus sugar for `Devicetree`/`Pci`/`Amba` now and rejects the rest as
/// "not yet supported" (GRILL D08/C03). Closed, append-only at the end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchEntry {
    /// `compatible = ["vendor,device"]` (devicetree / OF).
    Devicetree { compatible: StringId },
    /// `pci = [{ vendor, device, [subsystem], [class] }]`.
    Pci {
        vendor: u16,
        device: u16,
        subsystem: Option<u32>,
        class: Option<u32>,
    },
    /// `amba = [{ id, mask }]` (ARM AMBA periphid + mask).
    Amba { id: u32, mask: u32 },
    /// ACPI HID/CID (sugar deferred).
    Acpi { hid: StringId },
    /// USB vendor/product (sugar deferred).
    Usb { vendor: u16, product: u16 },
    /// I2C device name (sugar deferred).
    I2c { name: StringId },
    /// SPI device name (sugar deferred).
    Spi { name: StringId },
}

/// Device-class tag. Open vocabulary; default `Generic`. The class-trait
/// cross-check is deferred to the Device Class construct (GRILL D06).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassTag {
    /// No class-specific public interface (default).
    Generic,
    /// Serial device class.
    Serial,
    /// Network device class.
    Network,
    /// Any other tag, carried verbatim.
    Other(StringId),
}

impl Default for ClassTag {
    fn default() -> Self {
        ClassTag::Generic
    }
}

/// The five optional lifecycle callbacks.
///
/// `Some(InstanceRef)` = the author overrode this callback and the bridge
/// force-materialized its body (the backend synthesizes a call to it).
/// `None` = the trait default is used: for the four "glued" callbacks the
/// backend still emits the `LC call <cb>` marker (the glue ran) but synthesizes
/// no author call; for `on_system_shutdown` the whole slot is omitted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleOps {
    /// `on_init` — after preamble (bring-up).
    pub on_init: Option<InstanceRef>,
    /// `on_deinit` — orderly teardown (HW reachable).
    pub on_deinit: Option<InstanceRef>,
    /// `on_surprise_remove` — teardown, device gone (no HW access).
    pub on_surprise_remove: Option<InstanceRef>,
    /// `on_init_failure` — `on_init` returned `Err`; partial-state cleanup.
    pub on_init_failure: Option<InstanceRef>,
    /// `on_system_shutdown` — system going down; slot omitted when `None`.
    pub on_system_shutdown: Option<InstanceRef>,
}

/// The spec-view driver-struct fields, split into compiler-bound resources and
/// non-resource user-data (the latter Rule-1/Rule-2 classified). Empty for a
/// fieldless driver (the M0 walking skeleton).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverHandle {
    /// Resource fields (`Mmio<T>`, …) — other constructs own their setup; the
    /// lifecycle glue runs preamble/postamble around them in declaration order.
    pub resource_fields: Vec<ResourceField>,
    /// Non-resource user-data fields (Rule-1/Rule-2).
    pub user_data_fields: Vec<UserDataField>,
}

/// A resource field on the driver struct (declaration order preserved).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceField {
    /// The struct field name.
    pub name: StringId,
    /// The resource kind (its setup is owned by another construct).
    pub kind: ResourceKind,
}

/// Closed, append-only resource-family taxonomy. Only `Mmio` exists now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    /// `Mmio<T>` register-access resource.
    Mmio,
}

/// A non-resource user-data field, classified per `user-data-initialization.md`
/// (GRILL C10). Build-now is scalar-only; non-scalar shapes are rejected at
/// construction (diagnostic), so they never reach this table this stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDataField {
    /// The struct field name.
    pub name: StringId,
    /// The field's type.
    pub ty: TyId,
    /// Rule-1 (zero-init) vs Rule-2 (`Option<T: Copy>`) classification.
    pub kind: UserDataKind,
}

/// User-data init classification (GRILL C10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserDataKind {
    /// Rule 1: zero-init whitelist (the zeroed allocation IS the init).
    ZeroInit,
    /// Rule 2: `Option<T: Copy>` — `None` is the unseeded state.
    OptionCopy,
}

/// Serde adapter: serialize the `ItemId`-keyed driver map as a JSON array of
/// `(key, value)` pairs (mirrors `domain_attr::ordered_map_vec`). `BTreeMap`
/// iteration is key-sorted, so the output is deterministic across runs.
mod ordered_driver_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(map: &BTreeMap<ItemId, DriverLifecycle>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<(&ItemId, &DriverLifecycle)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<ItemId, DriverLifecycle>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(ItemId, DriverLifecycle)>::deserialize(deserializer)?;
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

    fn round_trip<T>(value: T)
    where
        T: Clone + core::fmt::Debug + PartialEq + Serialize + DeserializeOwned,
    {
        let encoded = serde_json::to_string(&value).expect("serialize");
        let decoded: T = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, value);
    }

    #[test]
    fn class_tag_defaults_to_generic() {
        assert_eq!(ClassTag::default(), ClassTag::Generic);
    }

    #[test]
    fn match_entry_variants_round_trip() {
        round_trip(MatchEntry::Devicetree {
            compatible: sid(1),
        });
        round_trip(MatchEntry::Pci {
            vendor: 0x8086,
            device: 0x100E,
            subsystem: None,
            class: None,
        });
        round_trip(MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff,
        });
    }

    #[test]
    fn lifecycle_ops_default_is_all_none() {
        let ops = LifecycleOps::default();
        assert!(ops.on_init.is_none());
        assert!(ops.on_deinit.is_none());
        assert!(ops.on_surprise_remove.is_none());
        assert!(ops.on_init_failure.is_none());
        assert!(ops.on_system_shutdown.is_none());
    }

    #[test]
    fn device_lifecycle_tables_round_trip() {
        let mut tables = DeviceLifecycleTables::default();
        tables.drivers.insert(
            ItemId::from_u32(3),
            DriverLifecycle {
                descriptor: DriverDescriptor {
                    name: sid(2),
                    match_set: vec![MatchEntry::Devicetree {
                        compatible: sid(1),
                    }],
                    device_class: ClassTag::Generic,
                },
                handle: DriverHandle::default(),
                ops: LifecycleOps::default(),
            },
        );
        round_trip(tables);
    }

    #[test]
    fn driver_map_json_is_deterministic_across_insert_order() {
        let mk = |item: u32, name: u32| {
            (
                ItemId::from_u32(item),
                DriverLifecycle {
                    descriptor: DriverDescriptor {
                        name: sid(name),
                        match_set: vec![MatchEntry::Amba {
                            id: 1,
                            mask: 0xffff,
                        }],
                        device_class: ClassTag::Serial,
                    },
                    handle: DriverHandle::default(),
                    ops: LifecycleOps::default(),
                },
            )
        };
        let entries = [mk(9, 10), mk(2, 11), mk(5, 12)];

        let mut a = DeviceLifecycleTables::default();
        for (k, v) in entries.iter() {
            a.drivers.insert(*k, v.clone());
        }
        let mut b = DeviceLifecycleTables::default();
        for (k, v) in entries.iter().rev() {
            b.drivers.insert(*k, v.clone());
        }
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}
