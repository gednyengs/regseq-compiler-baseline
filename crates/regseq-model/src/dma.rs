//! DMA-management construct IR (dma-management).
//!
//! The author declares coherent `DmaRegion<T>` and slave `DmaChannel` fields on
//! the `#[rseq::driver]` struct (via `#[rseq::dma_region(…)]` /
//! `#[rseq::dma_channel(…)]`), maps CPU buffers with `dma::stream`, and binds a
//! completion handler with `#[rseq::dma_completion(channel = …)]`. This module is
//! the owned/serializable model of that surface: per-resource facts, per-
//! `element_access`-site facts (the C09 carrier — index, view type, the two-bit
//! effect carrier, lexical extent), the `DmaStreamBuffer` RAII lifetime carrier,
//! the `DmaRegionSpan` endpoints at `stage_transfer` sites, the completion
//! bindings, and the `dma_mmio_store_order` edge as concrete endpoints. It is
//! populated by the `regseq-driver` bridge's `dma` recognition, joined by the
//! `regseq-l2` `dma` tenant, and consumed by the generic-c backend's lifecycle
//! glue + per-op lowering.
//!
//! It is the `clock.rs` / `pinctrl.rs` mirror (a dedicated top-level
//! `RegseqModel` table, GRILL C09): a DMA region/channel is a *capability layered
//! on the device* (memory it shares / a transport engine it drives), not part of
//! the `DriverHandle`'s address footprint, so it gets its own table rather than
//! living on `DriverHandle`. Lifecycle adjacency (region `alloc_coherent` /
//! channel `request_channel` bound to device lifetime, LIFO teardown) is handled
//! by phase-ordering across tables, not by IR co-location.
//!
//! IR-layer scope (this chunk): the data shapes only. Recognition (bridge), the
//! L2 `[dma.<id>]` join, and backend lowering land in later chunks. A few field
//! representations are NOVEL with no clock/pinctrl precedent — the
//! `LexicalExtent` of an `element_access` closure and the concrete `store_order`
//! endpoints — and are carried here as reasonable typed placeholders that
//! round-trip serde; they are FLAGGED for backend wiring later (see the per-type
//! docs).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{BlockId, ItemId, StringId};
use crate::domain_attr::{DmaFlow, ElementVerb};
use crate::source::Span;

/// Resolved layout facts for an element/view type `T`/`V` (dma-management
/// FIX #2): the rustc-computed ABI `size`/`align` plus a conservative M1 POD
/// marker. Carried in the DMA IR so the declaration-time layout guard and the
/// (M4) per-access size-fits-view verify rule (PLAN T19) read concrete numbers
/// rather than re-deriving from a bare type name. `size`/`align` are bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaLayoutFacts {
    /// The ABI size of the type in bytes (`size_of::<T>()`).
    pub size: u64,
    /// The ABI alignment of the type in bytes (`align_of::<T>()`).
    pub align: u64,
    /// A conservative M1 plain-old-data marker: `true` when the type has NO Drop
    /// glue (`!needs_drop`). A coherent DMA buffer must hold trivially-copyable
    /// bytes; a type with a destructor is not safe to memcpy through the backing
    /// store. (M1 proxy: the full descriptor-POD verify is M4/T19.)
    pub is_pod: bool,
}

/// Sibling table on `RegseqModel` for recognized DMA surfaces, keyed by the
/// `#[rseq::driver]` struct's final `ItemId` (assigned at Phase 4, like
/// `clock` / `pinctrl` / `interrupt`). Sparse via map absence. `#[serde(default)]`
/// on the model field keeps pre-construct models deserializable (GRILL C09/R15).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaTables {
    /// Recognized per-driver DMA surfaces, keyed by the driver-struct Item.
    #[serde(with = "ordered_dma_map")]
    pub drivers: BTreeMap<ItemId, DriverDma>,
}

/// One driver's DMA surface: its coherent regions, slave channels, streaming
/// buffers, and completion bindings, each in declaration/site order (the
/// lifecycle preambles run in declaration order, like `DriverClocks`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverDma {
    /// `#[rseq::dma_region(…)]` coherent regions, declaration order.
    pub regions: Vec<DmaRegionFacts>,
    /// `#[rseq::dma_channel(…)]` slave channels, declaration order.
    pub channels: Vec<DmaChannelFacts>,
    /// `dma::stream(buf, flow)` streaming buffers, site order.
    pub streams: Vec<DmaStreamFacts>,
    /// `region.span(range)` endpoints recorded at `stage_transfer` call-sites
    /// (dma-management M3/T09, GRILL D05/C06), site order. Each carries the
    /// element-indexed device address + byte length + region provenance. Added
    /// in M3; `#[serde(default)]` keeps pre-M3 models deserializable.
    #[serde(default)]
    pub spans: Vec<DmaRegionSpan>,
    /// `#[rseq::dma_completion(channel = …)]` completion bindings, declaration
    /// order.
    pub completions: Vec<CompletionBinding>,
}

/// One recognized coherent `DmaRegion<T>` resource field on the driver struct
/// (GRILL C09). Carries the declaration constants, the captured element type, the
/// per-`element_access`-site facts, the `stage_transfer` span endpoints sourced
/// from this region, the `store_order` edges it publishes, and the L2 binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaRegionFacts {
    /// The struct field name.
    pub name: StringId,
    /// The mandatory L1 resource id (`#[rseq::dma_region(id = "…")]`): the join
    /// key to `[dma.<id>]` and the generic-c trace identity.
    pub id: StringId,
    /// The element stride in bytes (`element_size`).
    pub element_size: u64,
    /// The element cardinality (`element_count`).
    pub element_count: u64,
    /// The explicit `alignment` (bytes), or `None` when omitted (defaults to
    /// `align_of::<T>()` at a later layout pass, GRILL D08/C03).
    pub alignment: Option<u64>,
    /// The interned element type `T` name captured at recognition (the layout
    /// verify rule and the typed `element_access` lowering read it). Empty until
    /// recognition captures `T`.
    #[serde(default)]
    pub element_type: Option<StringId>,
    /// The resolved ABI layout (size/align/POD) of the element type `T`
    /// (dma-management FIX #2). `None` when `T`'s layout is unresolvable at
    /// recognition (the declaration-time layout guard then SKIPs rather than
    /// crashing). `#[serde(default)]` keeps earlier models deserializable.
    #[serde(default)]
    pub element_layout: Option<DmaLayoutFacts>,
    /// Per-`element_access`-site facts on this region (GRILL C09): each closure
    /// borrow's index, view type, effects, and lexical extent.
    #[serde(default)]
    pub element_accesses: Vec<ElementAccessFact>,
    /// The L2-resolved `[dma.<id>]` region binding. Empty (default) until the
    /// platform-file join. `#[serde(default)]` keeps earlier models
    /// deserializable.
    #[serde(default)]
    pub binding: DmaRegionBinding,
}

/// One recognized slave `DmaChannel` resource field on the driver struct
/// (GRILL C09).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaChannelFacts {
    /// The struct field name.
    pub name: StringId,
    /// The mandatory L1 resource id (`#[rseq::dma_channel(id = "…")]`).
    pub id: StringId,
    /// The declared transfer direction.
    pub direction: DmaFlow,
    /// The L2-resolved `[dma.<id>]` channel binding. Empty (default) until the
    /// platform-file join.
    #[serde(default)]
    pub binding: DmaChannelBinding,
}

/// One recognized `dma::stream(buf, flow)` streaming-buffer site (GRILL C09/S03).
/// Carries the streaming flow and the RAII lifetime carrier (the buffer is
/// unmapped on Drop).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaStreamFacts {
    /// The streaming direction (`flow`).
    pub flow: DmaFlow,
    /// The RAII lifetime of the mapped buffer: `dma::stream` maps it, and the
    /// implicit Drop at the end of `lifetime` unmaps it (GRILL R05/S03/C09/C10).
    pub lifetime: DmaStreamBuffer,
}

/// The RAII lifetime carrier of a `DmaStreamBuffer` (GRILL R05/S03). The buffer
/// is streaming-mapped at `dma::stream` and unmapped at the end of its lexical
/// lifetime (Rust Drop). The carrier records the lexical extent over which the
/// mapping is live so the backend can place the unmap hook at the Drop point.
///
/// PLACEHOLDER-FLAGGED: the exact Drop-seam representation has no clock/pinctrl
/// precedent; the lexical extent is carried as a [`LexicalExtent`] (see its docs)
/// and the precise drop block/local will be wired when the backend emit lands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaStreamBuffer {
    /// The lexical extent over which the streaming mapping is live (map at entry,
    /// unmap at exit).
    pub extent: LexicalExtent,
}

/// Per-`element_access`-site facts (GRILL C09): the C01 carrier for one
/// `region.<verb>(i, |&[mut] V| …)` closure borrow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementAccessFact {
    /// The interned region resource id this access targets (the join key back to
    /// `DmaRegionFacts.id`).
    pub region: StringId,
    /// The element index `i` (where statically known); `None` for a non-constant
    /// index (the OOB `i in [0, count)` check then defers, GRILL C03).
    #[serde(default)]
    pub index: Option<u64>,
    /// The interned view type `V` name (`region.<verb>::<V>(i, …)`; `V = T` by
    /// default or an explicit per-call reinterpret `U`). The layout verify rule
    /// reads it (GRILL D08/C03). Empty until recognition captures `V`.
    #[serde(default)]
    pub view_type: Option<StringId>,
    /// The resolved ABI layout (size/align/POD) of the view type `V`
    /// (dma-management FIX #2): the region's `element_layout` when `V = T` (or no
    /// turbofish), else `V`'s own size/align from the model type table. `None`
    /// when unresolvable. The (M4/T19) per-access size-fits-view verify reads it;
    /// M1 only carries it. `#[serde(default)]` keeps earlier models
    /// deserializable.
    #[serde(default)]
    pub view_layout: Option<DmaLayoutFacts>,
    /// The access verb (`read`/`write`/`update`).
    pub verb: ElementVerb,
    /// The two-bit effect carrier `{consumes_device_data, publishes_cpu_data}`,
    /// DERIVED from the verb (GRILL C01/D02). Stored so a consumer reads the bits
    /// without re-deriving; equals `verb.effects()`.
    pub effects: AccessEffects,
    /// The lexical extent of the closure borrow (entry/exit brackets the
    /// ordering/cache obligation, GRILL D01/C01).
    pub lexical_extent: LexicalExtent,
}

/// The two-bit access effect carrier `{consumes_device_data, publishes_cpu_data}`
/// (GRILL C01/D02). Derived from an [`ElementVerb`] via [`ElementVerb::effects`]:
/// `read => {1,0}`, `write => {0,1}`, `update => {1,1}`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccessEffects {
    /// The closure consumes device-written data (invalidate-before on
    /// `per_access`).
    pub consumes_device_data: bool,
    /// The closure publishes CPU-written data (flush-after on `per_access`).
    pub publishes_cpu_data: bool,
}

impl AccessEffects {
    /// Build the carrier from a verb (the single source of truth, GRILL C01).
    pub fn from_verb(verb: ElementVerb) -> Self {
        let (consumes_device_data, publishes_cpu_data) = verb.effects();
        Self {
            consumes_device_data,
            publishes_cpu_data,
        }
    }
}

/// A `DmaRegionSpan` recorded at a `stage_transfer` call-site (GRILL C09/C06):
/// the non-borrowing, direction-neutral element-indexed view of a coherent
/// region handed to a channel transport op. `device_addr = base + start *
/// element_size`, `len = range.len() * element_size`, `provenance = region`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaRegionSpan {
    /// The synthetic device address of the span (`base + start*element_size`).
    /// The concrete base is L2/runtime-resolved; recorded here as the element-
    /// offset form `start*element_size` until the base is joined.
    pub device_addr: SpanAddr,
    /// The span length in bytes (`range.len() * element_size`).
    pub len: u64,
    /// The provenance: the interned region resource id this span views (the join
    /// key back to `DmaRegionFacts.id`, GRILL C06).
    pub provenance: StringId,
}

/// The device address of a [`DmaRegionSpan`], recorded as the element-offset form
/// until the L2/runtime base is joined (`addr = region_base + element_offset *
/// element_size`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanAddr {
    /// The element offset (`range.start`) into the provenance region.
    pub element_offset: u64,
    /// The element stride in bytes (`element_size`) the offset multiplies, copied
    /// from the region so the byte address is computable without re-joining.
    pub element_size: u64,
}

/// A recognized completion binding (GRILL C09/C05; registration shape only this
/// cycle, D06). Registration metadata; the host model records it but never fires.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionBinding {
    /// The interned channel id this handler completes (the join key back to
    /// `DmaChannelFacts.id`).
    pub channel: StringId,
    /// The completion-handler method's Item.
    pub handler: ItemId,
    /// A marker for the `DmaCompletion` payload type (interned name / framework-
    /// type id). Registration metadata; never constructed/delivered this cycle.
    pub payload: StringId,
}

/// The `dma_mmio_store_order` edge AS CONCRETE ENDPOINTS (GRILL C09/C07/D09): the
/// last publishing region store op and the MMIO write (doorbell) it publishes.
/// The fence-insertion pass emits `regseq_dma_wmb()` between them.
///
/// PLACEHOLDER-FLAGGED: the precise endpoint identity (exact statement/operand)
/// has no clock/pinctrl precedent. The endpoints are carried as
/// [`StoreOrderEndpoint`]s pinning the basic block + a statement index; the exact
/// store-op-vs-doorbell-write trigger analysis ("which store is the LAST
/// publishing store") is `[Open]` in design §6 and will be wired when the
/// fence-emission backend lands (PLAN T16). Round-trips serde as-is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreOrderEdge {
    /// The interned region resource id whose stores are being fenced (the join
    /// key back to `DmaRegionFacts.id`).
    pub region: StringId,
    /// The last publishing region-store endpoint (the source of the edge).
    pub region_store_op: StoreOrderEndpoint,
    /// The MMIO doorbell-write endpoint that exposes the stores (the sink of the
    /// edge; register-access owns the plain write, dma-management inserts only the
    /// fence between them, GRILL C07).
    pub mmio_write: StoreOrderEndpoint,
}

/// One endpoint of a [`StoreOrderEdge`], pinned as a `(block, statement-index)`
/// location (GRILL C09/D09). PLACEHOLDER-FLAGGED: a reasonable typed carrier that
/// round-trips serde; the exact node identity is wired with the fence-emission
/// pass (PLAN T16).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StoreOrderEndpoint {
    /// The basic block containing the endpoint statement/terminator.
    pub block: BlockId,
    /// The statement index within `block` (the terminator is `statement ==
    /// block.statements.len()`).
    pub statement: u32,
}

/// The lexical extent of a closure borrow / streaming-buffer lifetime, recorded
/// so the backend can bracket the ordering/cache obligation (or place the RAII
/// unmap) at entry/exit (GRILL D01/C01).
///
/// PLACEHOLDER-FLAGGED: the `element_access` lexical-extent IR carrier has NO
/// clock/pinctrl precedent (the highest-novelty field, PLAN R-closure-extent).
/// Design §6 asserts a captures-struct + call-shim lowering; until that backend
/// wiring lands, the extent is carried as a source [`Span`] (the natural typed
/// extent that round-trips serde). The precise entry/exit block pair will be
/// attached when the closure-borrow codegen is built (PLAN T06/T13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LexicalExtent {
    /// The source span bracketing the borrow/lifetime (entry .. exit).
    pub span: Span,
}

/// The L2-resolved `[dma.<id>]` facts for a coherent region (GRILL C02). Empty
/// (default) until the platform-file join; filled by the `dma` L2 tenant in a
/// later chunk. `#[serde(default)]` keeps earlier serialized models
/// deserializable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaRegionBinding {
    /// `device_address_bits` (was `dma_mask`); `None` until joined.
    pub device_address_bits: Option<u32>,
    /// `alignment` (bytes); `None` ⇒ `align_of::<T>()` (NOT 1, GRILL D08/C03).
    pub alignment: Option<u64>,
    /// `region_cache_maintenance = none | per_access` gate (GRILL D03/C02);
    /// defaults to `None` (backend-safe `none`) until joined.
    pub cache_maintenance: Option<RegionCacheMaintenance>,
    /// Whether the `[dma.<id>]` region section was present and joined.
    pub resolved: bool,
}

/// `region_cache_maintenance` gate (GRILL D03/C02). Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegionCacheMaintenance {
    /// `none` — coherent memory; emit no cache op. The backend-safe default.
    #[default]
    None = 0,
    /// `per_access` — flush/invalidate per the verb (REQUIRES cacheline
    /// isolation, FM4/D03). Recorded only this cycle (not enforced).
    PerAccess = 1,
}

/// The L2-resolved `[dma.<id>]` facts for a slave channel (GRILL C02). Empty
/// (default) until the platform-file join.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaChannelBinding {
    /// `endpoint_addr_width` in bytes (was `slave_addr_width`); `None` until
    /// joined.
    pub endpoint_addr_width: Option<u32>,
    /// `endpoint_max_burst_beats` (was `slave_maxburst`); `None` until joined.
    pub endpoint_max_burst_beats: Option<u32>,
    /// Whether the `[dma.<id>]` channel section was present and joined.
    pub resolved: bool,
}

/// Serde adapter: serialize the `ItemId`-keyed driver map as a JSON array of
/// `(key, value)` pairs (mirrors `clock::ordered_clock_map` /
/// `pinctrl::ordered_pinctrl_map`). `BTreeMap` iteration is key-sorted, so the
/// output is deterministic across runs regardless of insertion order.
mod ordered_dma_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(map: &BTreeMap<ItemId, DriverDma>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<(&ItemId, &DriverDma)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<ItemId, DriverDma>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(ItemId, DriverDma)>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::SourceFileId;
    use serde::de::DeserializeOwned;

    fn sid(n: u32) -> StringId {
        StringId::from_u32(n)
    }

    fn span(lo: u32, hi: u32) -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo,
            hi,
        }
    }

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
        let json = serde_json::to_string(value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&back, value);
    }

    fn sample_tables() -> DmaTables {
        let mut tables = DmaTables::default();
        tables.drivers.insert(
            ItemId::from_u32(3),
            DriverDma {
                regions: vec![DmaRegionFacts {
                    name: sid(1),
                    id: sid(2),
                    element_size: 16,
                    element_count: 256,
                    alignment: Some(16),
                    element_type: Some(sid(20)),
                    element_layout: Some(DmaLayoutFacts {
                        size: 16,
                        align: 8,
                        is_pod: true,
                    }),
                    element_accesses: vec![
                        ElementAccessFact {
                            region: sid(2),
                            index: Some(0),
                            view_type: Some(sid(20)),
                            view_layout: Some(DmaLayoutFacts {
                                size: 16,
                                align: 8,
                                is_pod: true,
                            }),
                            verb: ElementVerb::Write,
                            effects: AccessEffects::from_verb(ElementVerb::Write),
                            lexical_extent: LexicalExtent { span: span(10, 40) },
                        },
                        ElementAccessFact {
                            region: sid(2),
                            index: None,
                            view_type: Some(sid(21)),
                            view_layout: None,
                            verb: ElementVerb::Update,
                            effects: AccessEffects::from_verb(ElementVerb::Update),
                            lexical_extent: LexicalExtent { span: span(50, 80) },
                        },
                    ],
                    binding: DmaRegionBinding {
                        device_address_bits: Some(32),
                        alignment: Some(16),
                        cache_maintenance: Some(RegionCacheMaintenance::None),
                        resolved: true,
                    },
                }],
                channels: vec![DmaChannelFacts {
                    name: sid(5),
                    id: sid(6),
                    direction: DmaFlow::DeviceToHost,
                    binding: DmaChannelBinding {
                        endpoint_addr_width: Some(1),
                        endpoint_max_burst_beats: Some(16),
                        resolved: true,
                    },
                }],
                streams: vec![DmaStreamFacts {
                    flow: DmaFlow::HostToDevice,
                    lifetime: DmaStreamBuffer {
                        extent: LexicalExtent { span: span(90, 120) },
                    },
                }],
                spans: vec![DmaRegionSpan {
                    device_addr: SpanAddr {
                        element_offset: 0,
                        element_size: 16,
                    },
                    len: 64,
                    provenance: sid(2),
                }],
                completions: vec![CompletionBinding {
                    channel: sid(6),
                    handler: ItemId::from_u32(9),
                    payload: sid(30),
                }],
            },
        );
        tables
    }

    #[test]
    fn dma_tables_round_trip() {
        round_trip(&sample_tables());
    }

    #[test]
    fn dma_map_json_is_deterministic_across_insert_order() {
        let mut a = DmaTables::default();
        let mut b = DmaTables::default();
        let mk = |n: u32| DriverDma {
            regions: vec![DmaRegionFacts {
                name: sid(n),
                id: sid(n + 1),
                element_size: 8,
                element_count: 64,
                alignment: None,
                element_type: None,
                element_layout: None,
                element_accesses: vec![],
                binding: DmaRegionBinding::default(),
            }],
            channels: vec![],
            streams: vec![],
            spans: vec![],
            completions: vec![],
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
    fn access_effects_derive_from_verb() {
        // The two-bit carrier is derived from the verb (GRILL C01/D02):
        // read={1,0}, write={0,1}, update={1,1}.
        assert_eq!(
            AccessEffects::from_verb(ElementVerb::Read),
            AccessEffects {
                consumes_device_data: true,
                publishes_cpu_data: false,
            }
        );
        assert_eq!(
            AccessEffects::from_verb(ElementVerb::Write),
            AccessEffects {
                consumes_device_data: false,
                publishes_cpu_data: true,
            }
        );
        assert_eq!(
            AccessEffects::from_verb(ElementVerb::Update),
            AccessEffects {
                consumes_device_data: true,
                publishes_cpu_data: true,
            }
        );
        // The carrier agrees with the source-of-truth `ElementVerb::effects`.
        for verb in [ElementVerb::Read, ElementVerb::Write, ElementVerb::Update] {
            let (c, p) = verb.effects();
            let eff = AccessEffects::from_verb(verb);
            assert_eq!((eff.consumes_device_data, eff.publishes_cpu_data), (c, p));
        }
    }

    #[test]
    fn region_cache_maintenance_defaults_none_and_discriminants_stable() {
        assert_eq!(RegionCacheMaintenance::default(), RegionCacheMaintenance::None);
        // Fieldless `repr(u8)` enum — `as u8` yields the pinned tag.
        assert_eq!(RegionCacheMaintenance::None as u8, 0);
        assert_eq!(RegionCacheMaintenance::PerAccess as u8, 1);
    }

    #[test]
    fn dma_flow_discriminants_are_stable() {
        // The direction enum lives in domain_attr; pin its serialized tags here so
        // the dma construct's discriminant cover is complete. Fieldless `repr(u8)`.
        assert_eq!(DmaFlow::DeviceToHost as u8, 0);
        assert_eq!(DmaFlow::HostToDevice as u8, 1);
    }

    #[test]
    fn store_order_edge_round_trips() {
        // The store_order edge carries concrete endpoints (GRILL C09/D09);
        // round-trips serde as a placeholder pending backend wiring (PLAN T16).
        let edge = StoreOrderEdge {
            region: sid(2),
            region_store_op: StoreOrderEndpoint {
                block: BlockId::from_u32(1),
                statement: 3,
            },
            mmio_write: StoreOrderEndpoint {
                block: BlockId::from_u32(1),
                statement: 7,
            },
        };
        round_trip(&edge);
    }

    #[test]
    fn region_span_round_trips() {
        let span_view = DmaRegionSpan {
            device_addr: SpanAddr {
                element_offset: 4,
                element_size: 16,
            },
            len: 64,
            provenance: sid(2),
        };
        round_trip(&span_view);
    }

    #[test]
    fn stream_buffer_lifetime_round_trips() {
        // The DmaStreamBuffer RAII lifetime carrier round-trips (GRILL R05/S03).
        let buf = DmaStreamBuffer {
            extent: LexicalExtent { span: span(0, 10) },
        };
        round_trip(&buf);
    }
}
