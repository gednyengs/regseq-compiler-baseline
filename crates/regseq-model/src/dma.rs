//! Locked DMA-management IR.
//!
//! The two admitted L1 elements are deliberately independent: an owned
//! `DmaBuffer` and an exclusively owned `AsyncTransferChannel`. Missing
//! operations remain absent capabilities; the model never invents an address,
//! status, control result, or completion notification.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{InstanceRef, ItemId};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaTables {
    #[serde(with = "ordered_dma_map")]
    pub drivers: BTreeMap<ItemId, DriverDma>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverDma {
    pub buffer: Option<OwnedDmaBuffer>,
    pub channel: Option<AsyncTransferChannel>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedDmaBuffer {
    pub ops: DmaBufferOps,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaBufferOps {
    pub allocate: Option<InstanceRef>,
    pub free: Option<InstanceRef>,
    pub cpu_address: Option<InstanceRef>,
    pub length: Option<InstanceRef>,
    pub transfer_address: Option<InstanceRef>,
    pub set_valid_length: Option<InstanceRef>,
}

impl DmaBufferOps {
    pub fn capabilities(&self) -> impl Iterator<Item = &'static str> {
        [
            ("allocate", self.allocate.is_some()),
            ("free", self.free.is_some()),
            ("cpu_address", self.cpu_address.is_some()),
            ("length", self.length.is_some()),
            ("transfer_address", self.transfer_address.is_some()),
            ("set_valid_length", self.set_valid_length.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, present)| present.then_some(name))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsyncTransferChannel {
    pub ops: DmaChannelOps,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaChannelOps {
    pub acquire: Option<InstanceRef>,
    pub configure: Option<InstanceRef>,
    pub prepare: Option<InstanceRef>,
    pub submit: Option<InstanceRef>,
    pub activate: Option<InstanceRef>,
    pub release: Option<InstanceRef>,
    pub status: Option<InstanceRef>,
    pub pause: Option<InstanceRef>,
    pub resume: Option<InstanceRef>,
    pub suspend: Option<InstanceRef>,
    pub reload: Option<InstanceRef>,
    pub terminate: Option<InstanceRef>,
    pub synchronize: Option<InstanceRef>,
    pub reclaim: Option<InstanceRef>,
    pub on_completion: Option<InstanceRef>,
}

impl DmaChannelOps {
    pub fn capabilities(&self) -> impl Iterator<Item = &'static str> {
        [
            ("acquire", self.acquire.is_some()),
            ("configure", self.configure.is_some()),
            ("prepare", self.prepare.is_some()),
            ("submit", self.submit.is_some()),
            ("activate", self.activate.is_some()),
            ("release", self.release.is_some()),
            ("status", self.status.is_some()),
            ("pause", self.pause.is_some()),
            ("resume", self.resume.is_some()),
            ("suspend", self.suspend.is_some()),
            ("reload", self.reload.is_some()),
            ("terminate", self.terminate.is_some()),
            ("synchronize", self.synchronize.is_some()),
            ("reclaim", self.reclaim.is_some()),
            ("on_completion", self.on_completion.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, present)| present.then_some(name))
    }
}

mod ordered_dma_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(map: &BTreeMap<ItemId, DriverDma>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        map.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<ItemId, DriverDma>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Vec::<(ItemId, DriverDma)>::deserialize(deserializer)?
            .into_iter()
            .collect())
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
                stable_crate_id: StableCrateId(9),
                local_hash: n,
            },
            args_key: ArgsKey(0),
            fn_shim_disambig: 0,
        }
    }

    #[test]
    fn dma_two_element_model_roundtrip_and_absence() {
        let mut tables = DmaTables::default();
        tables.drivers.insert(
            ItemId::from_u32(1),
            DriverDma {
                buffer: Some(OwnedDmaBuffer {
                    ops: DmaBufferOps {
                        allocate: Some(op(1)),
                        free: Some(op(2)),
                        cpu_address: Some(op(3)),
                        length: Some(op(4)),
                        ..DmaBufferOps::default()
                    },
                }),
                channel: Some(AsyncTransferChannel {
                    ops: DmaChannelOps {
                        acquire: Some(op(5)),
                        release: Some(op(6)),
                        ..DmaChannelOps::default()
                    },
                }),
            },
        );
        let json = serde_json::to_string(&tables).unwrap();
        let back: DmaTables = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tables);
        let dma = &back.drivers[&ItemId::from_u32(1)];
        assert!(dma.buffer.as_ref().unwrap().ops.transfer_address.is_none());
        assert!(dma.channel.as_ref().unwrap().ops.on_completion.is_none());
    }

    #[test]
    fn dma_terminate_and_synchronize_are_independent_capabilities() {
        let ops = DmaChannelOps {
            terminate: Some(op(1)),
            synchronize: Some(op(2)),
            ..DmaChannelOps::default()
        };
        assert_eq!(
            ops.capabilities().collect::<Vec<_>>(),
            vec!["terminate", "synchronize"]
        );
        assert!(ops.on_completion.is_none());
    }
}
