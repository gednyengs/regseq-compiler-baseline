//! Locked network-interface-management IR.
//!
//! One driver-keyed element carries the portable interface operations. Packet
//! representation, descriptor rings and indices, DMA mappings, transport
//! endpoints, interrupt lines, and deferred execution remain in their existing
//! owners and are deliberately absent here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{InstanceRef, ItemId};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterfaceTables {
    #[serde(with = "ordered_network_map")]
    pub drivers: BTreeMap<ItemId, DriverNetworkInterface>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverNetworkInterface {
    pub ops: NetworkInterfaceOps,
}

/// Authored operation bodies. Required operations define the portable core;
/// `None` on an optional operation means unsupported, never a successful no-op
/// or a fabricated event/completion.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterfaceOps {
    pub set_published: Option<InstanceRef>,
    pub set_admin_enabled: Option<InstanceRef>,
    pub transmit: Option<InstanceRef>,
    pub receive: Option<InstanceRef>,
    pub report_carrier: Option<InstanceRef>,
    pub report_dormant: Option<InstanceRef>,
    pub query_operational_state: Option<InstanceRef>,
    pub get_property: Option<InstanceRef>,
    pub set_property: Option<InstanceRef>,
    pub read_counters: Option<InstanceRef>,
    pub register_observer: Option<InstanceRef>,
    pub unregister_observer: Option<InstanceRef>,
}

impl NetworkInterfaceOps {
    pub fn capabilities(&self) -> impl Iterator<Item = &'static str> {
        [
            ("set_published", self.set_published.is_some()),
            ("set_admin_enabled", self.set_admin_enabled.is_some()),
            ("transmit", self.transmit.is_some()),
            ("receive", self.receive.is_some()),
            ("report_carrier", self.report_carrier.is_some()),
            ("report_dormant", self.report_dormant.is_some()),
            (
                "query_operational_state",
                self.query_operational_state.is_some(),
            ),
            ("get_property", self.get_property.is_some()),
            ("set_property", self.set_property.is_some()),
            ("read_counters", self.read_counters.is_some()),
            ("register_observer", self.register_observer.is_some()),
            ("unregister_observer", self.unregister_observer.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, present)| present.then_some(name))
    }
}

mod ordered_network_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        map: &BTreeMap<ItemId, DriverNetworkInterface>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        map.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<ItemId, DriverNetworkInterface>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            Vec::<(ItemId, DriverNetworkInterface)>::deserialize(deserializer)?
                .into_iter()
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArgsKey, DefPathHash, StableCrateId};

    fn op(n: u64) -> InstanceRef {
        InstanceRef {
            regseq_kind_rank: 0,
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(11),
                local_hash: n,
            },
            args_key: ArgsKey(0),
            fn_shim_disambig: 0,
        }
    }

    #[test]
    fn network_full_surface_roundtrip_preserves_absences() {
        let mut tables = NetworkInterfaceTables::default();
        tables.drivers.insert(
            ItemId::from_u32(3),
            DriverNetworkInterface {
                ops: NetworkInterfaceOps {
                    set_admin_enabled: Some(op(1)),
                    transmit: Some(op(2)),
                    report_carrier: Some(op(3)),
                    query_operational_state: Some(op(4)),
                    get_property: Some(op(5)),
                    read_counters: Some(op(6)),
                    ..NetworkInterfaceOps::default()
                },
            },
        );
        let json = serde_json::to_string(&tables).unwrap();
        let back: NetworkInterfaceTables = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tables);
        let ops = &back.drivers[&ItemId::from_u32(3)].ops;
        assert!(ops.set_published.is_none());
        assert!(ops.receive.is_none());
        assert!(ops.register_observer.is_none());
        assert!(!json.contains("descriptor"));
        assert!(!json.contains("producer_index"));
    }
}
