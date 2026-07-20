//! Locked device-class-serial IR.
//!
//! A serial driver implements one `SerialPort` trait. Each authored override is
//! one capability; a missing override is preserved as an absent operation. The
//! model deliberately has no endpoint, interrupt, DMA, buffer, or register
//! ownership: those are L2/backend concerns.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, VariantIdx};
use crate::instance_ref::InstanceRef;

/// Per-driver serial-port tables.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialPortTables {
    /// Recognized serial ports, keyed by the driver struct.
    #[serde(with = "ordered_serial_port_map")]
    pub drivers: BTreeMap<ItemId, DriverSerialPort>,
}

/// One serial port and its capability-gated operation set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverSerialPort {
    /// Authored operation bodies. `None` means unsupported, never success.
    pub ops: SerialPortOps,
    /// Validated value-type bindings used by OS backends.
    pub types: SerialTypeBindings,
    /// Hardware-specific authored hooks plus their L2 capability gates. These
    /// are backend inputs and are deliberately excluded from `SerialPortOps`.
    #[serde(default)]
    pub hardware_ops: SerialHardwareOps,
}

/// Backend-only serial hardware hooks recognized from explicit capability
/// attributes. A hook is callable only when its matching L2 gate is enabled.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialHardwareOps {
    /// `serial-hardware-ops.break-send` authored hook.
    pub break_send: Option<InstanceRef>,
    /// Whether the platform binding enables `break_send` for this target.
    pub break_send_enabled: bool,
}

/// The locked serial-port operations. Transfers take ordinary pointer+length
/// buffer values and return the count actually moved; buffer ownership and
/// queueing remain outside this table. All operations are independently
/// capability-gated. In particular, `purge` and `abort` are distinct and an
/// absent `subscribe_events` means no terminal event is promised.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialPortOps {
    pub configure: Option<InstanceRef>,
    pub get_config: Option<InstanceRef>,
    pub transmit: Option<InstanceRef>,
    pub receive: Option<InstanceRef>,
    pub set_signals: Option<InstanceRef>,
    pub get_signals: Option<InstanceRef>,
    pub subscribe_events: Option<InstanceRef>,
    pub open: Option<InstanceRef>,
    pub close: Option<InstanceRef>,
    pub purge: Option<InstanceRef>,
    pub await_drain: Option<InstanceRef>,
    pub abort: Option<InstanceRef>,
}

impl SerialPortOps {
    /// Stable capability names, in locked operation order.
    pub fn capabilities(&self) -> impl Iterator<Item = &'static str> {
        [
            ("configure", self.configure.is_some()),
            ("get_config", self.get_config.is_some()),
            ("transmit", self.transmit.is_some()),
            ("receive", self.receive.is_some()),
            ("set_signals", self.set_signals.is_some()),
            ("get_signals", self.get_signals.is_some()),
            ("subscribe_events", self.subscribe_events.is_some()),
            ("open", self.open.is_some()),
            ("close", self.close.is_some()),
            ("purge", self.purge.is_some()),
            ("await_drain", self.await_drain.is_some()),
            ("abort", self.abort.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, present)| present.then_some(name))
    }

    pub fn is_empty(&self) -> bool {
        self.capabilities().next().is_none()
    }
}

/// Canonical semantic-to-authored-variant bindings for ordinary serial values.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialTypeBindings {
    pub status: Option<SerialEnumBinding>,
    pub data_bits: Option<SerialEnumBinding>,
    pub stop_bits: Option<SerialEnumBinding>,
    pub parity: Option<SerialEnumBinding>,
    pub flow_control: Option<SerialEnumBinding>,
    pub direction: Option<SerialEnumBinding>,
}

/// Authored variant indices in canonical semantic order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialEnumBinding {
    pub variants: Vec<VariantIdx>,
}

mod ordered_serial_port_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        map: &BTreeMap<ItemId, DriverSerialPort>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        map.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<ItemId, DriverSerialPort>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            Vec::<(ItemId, DriverSerialPort)>::deserialize(deserializer)?
                .into_iter()
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArgsKey, DefPathHash, StableCrateId};

    fn body(n: u64) -> InstanceRef {
        InstanceRef {
            regseq_kind_rank: 0,
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(7),
                local_hash: n,
            },
            args_key: ArgsKey(0),
            fn_shim_disambig: 0,
        }
    }

    #[test]
    fn serial_port_model_roundtrip_and_capabilities() {
        let mut tables = SerialPortTables::default();
        tables.drivers.insert(
            ItemId::from_u32(2),
            DriverSerialPort {
                ops: SerialPortOps {
                    transmit: Some(body(1)),
                    purge: Some(body(2)),
                    ..SerialPortOps::default()
                },
                types: SerialTypeBindings::default(),
                hardware_ops: SerialHardwareOps::default(),
            },
        );
        let json = serde_json::to_string(&tables).unwrap();
        let back: SerialPortTables = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tables);
        assert_eq!(
            back.drivers[&ItemId::from_u32(2)]
                .ops
                .capabilities()
                .collect::<Vec<_>>(),
            vec!["transmit", "purge"]
        );
    }

    #[test]
    fn abort_and_purge_are_independent_capabilities() {
        let ops = SerialPortOps {
            purge: Some(body(1)),
            ..SerialPortOps::default()
        };
        assert!(ops.purge.is_some());
        assert!(ops.abort.is_none());
        assert!(ops.subscribe_events.is_none());
    }
}
