# DMA Management

**Maturity: `candidate-prototype`** (dma-management M1–M5, generic-c only). The
E1000 coherent half + the PL011 channel subset are built and exercised; the
Linux/Zephyr lowering, completion delivery, the consume-order fence, and the
capability ops are **designed, not built this cycle** (see *Designed-not-built this
cycle* below). Names follow the **C08 OS-agnostic map** (GRILL D04/C08 +
`docs/constructs/dma-management/dma-management-taxonomy-amendments.md` §1) — no
Linux-API spellings on the L1 surface.

## Purpose

`DMA Management` defines the coherent-memory, streaming-mapping, DMA-channel, and
compiler-managed lifecycle/ordering surfaces used to lower DMA-capable drivers,
OS-agnostically.

This is a large mixed-form resource construct. It has a re-admitted completion
*registration* binding (event-source shape; delivery deferred), but no bound-handler
raw-IRQ callback surface (that stays with the future interrupt construct).

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `dma.region_id` | property | `#[rseq::dma_region(id = …)]` field attribute |
| `dma.element_size` | property | `#[rseq::dma_region(element_size = …)]` field attribute |
| `dma.element_count` | property | `#[rseq::dma_region(element_count = …)]` field attribute |
| `dma.alignment` | property | `#[rseq::dma_region(alignment = …)]` field attribute (optional; omitted ⇒ `align_of::<T>()`) |
| `dma.channel_id` | property | `#[rseq::dma_channel(id = …)]` field attribute |
| `dma.channel_direction` | property | `#[rseq::dma_channel(direction = …)]` field attribute (`device_to_host`/`host_to_device`) |
| `dma.dma_region` | resource | typed field `DmaRegion<T>` |
| `dma.dma_channel` | resource | typed field `DmaChannel` |
| `dma.dma_stream_buffer` | resource | `DmaStreamBuffer` (from `dma::stream`) |
| `dma.device_addr` | operation | `DmaRegion::device_addr()` / `device_addr_of(i)` (A2 addressing) |
| `dma.element_count_op` | operation | `DmaRegion::element_count()` (A2 introspection) |
| `dma.element_access` | operation | `DmaRegion::{read,write,update}(i, \|&[mut] V\| …)` — closure-scoped typed borrow |
| `dma.region_span` | operation | `DmaRegion::span(range) -> DmaRegionSpan` (direction-neutral endpoint) |
| `dma.streaming_map` | operation | `dma::stream(buf, flow) -> Result<DmaStreamBuffer, RegseqError>` |
| `dma.streaming_unmap` | operation | `DmaStreamBuffer` Drop / RAII unmap |
| `dma.acquire_from_device` | operation | `DmaStreamBuffer::acquire_from_device()` (ownership → CPU) |
| `dma.release_to_device` | operation | `DmaStreamBuffer::release_to_device()` (ownership → device) |
| `dma.channel_configure` | operation | `DmaChannel::configure(&self)` (parameterless; config is declaration/L2-derived) |
| `dma.channel_stage_transfer` | operation | `DmaChannel::stage_transfer(endpoint) -> Result<(), RegseqError>` (`DmaRegionSpan` \| `&DmaStreamBuffer`) |
| `dma.channel_start_transfer` | operation | `DmaChannel::start_transfer(&self) -> Result<(), RegseqError>` (merges submit + issue-pending) |
| `dma.channel_stop` | operation | `DmaChannel::stop(&self)` (infallible) |
| `dma.channel_remaining_bytes` | operation | `DmaChannel::remaining_bytes(&self) -> Result<usize, RegseqError>` |
| `dma.completion` | binding | `#[rseq::dma_completion(channel = …)]` handler (registration shape only) |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `dma.device_address_bits` | property | `[dma.<id>]` region binding field (was `dma_mask`) |
| `dma.alignment` | property | `[dma.<id>]` region binding field (bytes; omitted ⇒ `align_of::<T>()`) |
| `dma.region_cache_maintenance` | property | `[dma.<id>]` region gate (`none` \| `per_access`) |
| `dma.endpoint_addr_width` | property | `[dma.<id>]` channel binding field (was `slave_addr_width`) |
| `dma.endpoint_max_burst_beats` | property | `[dma.<id>]` channel binding field (was `slave_maxburst`) |
| `dma.binding_scope` | property | `[dma.<id>]` resolution scope (`concrete`\|`template`\|`family`\|`conditional`; capability gating, D07) |
| `dma.cap_*` | property | `[dma.<id>]` capability flags (`present`\|`absent`\|`unknown`; `absent` legal only in a concrete binding) |

### Compiler-managed

| Primitive | Role |
|---|---|
| `dma.alloc_coherent` | coherent region allocation/free around device lifetime |
| `dma.request_channel` | channel acquisition/release around device lifetime |
| `dma.store_order_fence` | the DMA↔MMIO `dma_mmio_store_order` fence (`regseq_dma_wmb`) between the last publishing region store and the MMIO doorbell |
| `dma.set_dma_mask` | configure the addressable bus-address width from binding/platform data (designed; generic-c no-op) |
| `dma.sg_construction` | compiler-internal scatter/gather construction (designed-not-built) |
| `dma.address_translation` | compiler-internal device-address translation |

## Accepted Surface Baseline

### Coherent-region baseline

Stable typed resource + the A2 addressing/access ops:

```rust
struct MyDriver {
    #[rseq::dma_region(id = "rx_ring", element_size = 32, element_count = 128)]
    rx_ring: DmaRegion<RxDesc>,
}

let base = self.rx_ring.device_addr();          // A2: region base
let n = self.rx_ring.element_count();            // A2: declaration const
self.rx_ring.write(0, |d: &mut RxDesc| { d.ctrl = 0; });  // closure-scoped typed borrow
```

Accepted decisions:

- `element_access` is a **closure-scoped** typed borrow (`read`/`write`/`update`),
  giving the compiler a lexical bracket for the ordering/cache obligation (D01); the
  access mode is the explicit verb, the two-bit effect carrier
  `{consumes_device_data, publishes_cpu_data}` is derived from it (D02).
- the per-call reinterpret view `V` (`write::<V>(…)`) and the declared `T` are both
  subject to the C03 layout verify rule (`V: Sized + DmaPod`, size/align/stride,
  static index in bounds).
- `device_addr()`/`device_addr_of(i)`/`element_count()` are A2 addressing/introspection
  ops, not `dma_present` (bus-master) triggers.

### Streaming-mapping baseline

Stable streaming handle:

```rust
let sb = dma::stream(buf, flow)?;       // -> Result<DmaStreamBuffer, RegseqError>
let addr = sb.device_addr();
sb.release_to_device();                 // ownership → device
sb.acquire_from_device();               // ownership → CPU
// RAII unmap at scope exit
```

Accepted decisions:

- `DmaStreamBuffer` is the stable streaming-DMA handle surface (was `DmaMapping`).
- stable handle methods: `device_addr()`, `acquire_from_device()` (ownership → CPU),
  `release_to_device()` (ownership → device) — OS-agnostic spellings (were
  `sync_for_cpu`/`sync_for_device`).
- `dma::stream(buf, flow) -> Result<DmaStreamBuffer>` (was `dma::map`); the `Result`
  encodes the mapping-error case and is `#[must_use]` + lint-enforced.
- unmap is RAII-driven through Drop; no separate raw-address-only stable surface.

### Channel-control baseline

Stable channel surface:

```rust
impl DmaChannel {
    fn configure(&self);
    fn stage_transfer(&self, endpoint: impl TransferEndpoint) -> Result<(), RegseqError>;
    fn start_transfer(&self) -> Result<(), RegseqError>;
    fn stop(&self);
    fn remaining_bytes(&self) -> Result<usize, RegseqError>;
}
```

Accepted decisions:

- `configure()` is parameterless; configuration is declaration/L2-derived.
- `stage_transfer(endpoint)` (was `prepare`) takes a `DmaRegionSpan` (from
  `region.span(range)`) or a `&DmaStreamBuffer` via the `TransferEndpoint` overload;
  the endpoint's source/destination role is fixed by the channel's declared direction.
- `start_transfer()` (was `submit` + `issue_pending`) is the stable merged surface.
- `stop()` is infallible; `remaining_bytes()` (was `query_residue`) is fallible
  (`-ENOSYS`/no-meaningful-residue ⇒ `Err`).
- the fallible ops' `Result` is `#[must_use]` + lint-enforced (a discarded `Result`,
  including `let _ = …`, is a compile-fail).

### Completion baseline (registration shape, re-admitted)

```rust
#[rseq::dma_completion(channel = "rx")]
fn rx_complete(&mut self, result: DmaCompletion) -> Result<(), RegseqError>;
```

Accepted decisions:

- dma-management owns the DMA-framework completion-callback *registration* (the
  generated `regseq_dma_register_completion`); the handler body is device-class logic.
- **registration shape only this cycle** — the host model records the binding but
  never fires the callback (D06); delivery is designed-not-built.
- raw device IRQ handler registration stays with the future interrupt construct.

### Ordering baseline

- `dma_mmio_store_order` (BUILT): a `regseq_dma_wmb()` fence between the last
  publishing region store and the MMIO doorbell that exposes it (register-access emits
  the plain doorbell; dma-management inserts only the fence).
- `completion_marker_order` (DESIGNED, NOT BUILT): the DD/phase consume fence
  (`regseq_dma_rmb`) — see *Designed-not-built this cycle*.

## Designed-not-built this cycle (honest deferrals)

The `candidate-prototype` maturity is generic-c only; these are designed (eval design
analysis + GRILL) but **not built / not exercised** this cycle:

- **Completion delivery/firing** (D06): only the registration shape is built; the host
  model never invokes the callback (no `DmaCompletion` is constructed/delivered).
- **`completion_marker_order` / `regseq_dma_rmb`** (D09): only the `store_order` fence
  is built; the consume-fence (FM1 marker-field identification, FM3 MMIO-sourced
  ordering) is not.
- **Capability ops + the concrete-`absent` cap-op rejection** (D07/S08): the cap-gated
  ops `pause`/`resume`/`stage_circular_transfer` (was `prepare_cyclic`)/`stage_segments`
  (was `prepare_sg`) are not built; only the L2 capability *contract* is built (a
  `cap_* = "absent"` in a non-concrete binding is rejected — DMA-L2-006). The
  concrete-`absent` op rejection has no op to reject yet.
- **Linux/Zephyr lowering + Tier-2** (model §7, design §5): the real-backend DMA
  lowering (Linux `dma_alloc_coherent`/`dmaengine_*`/`dma_wmb`; Zephyr stateful
  `dma_config`/`dma_start`/cache-op ordering) and a Tier-2 behavioral leg.
- **SG construction / `host_to_host` / partial-sync / slave asymmetry**: designed-in,
  not built.

## Extraction Baseline

The frontend must recognize:

- `#[rseq::dma_region(…)]` and `#[rseq::dma_channel(…)]` field attributes
- `#[rseq::dma_completion(channel = …)]` handler-method attribute (registration)
- `DmaRegion<T>`, `DmaChannel`, and `DmaStreamBuffer` typed resources
- the A2 ops (`device_addr`/`device_addr_of`/`element_count`), the `element_access`
  closure verbs (`read`/`write`/`update` + the per-call reinterpret `V`), and
  `region.span(range)`
- `dma::stream` + the `DmaStreamBuffer` handle flow (`acquire_from_device`/
  `release_to_device` + RAII unmap)
- the channel 5-op transport (`configure`/`stage_transfer`/`start_transfer`/`stop`/
  `remaining_bytes`)
- Layer 2 DMA binding data: `device_address_bits`, `alignment`,
  `region_cache_maintenance`, `endpoint_addr_width`, `endpoint_max_burst_beats`,
  `binding_scope`, `cap_*`

Required extraction validations:

- the C03 layout verify rule (declaration `T` + every reinterpret view `V`)
- the C10 fallibility lint (no discarded `Result` from the fallible ops)
- the anti-form / direction / span-bounds checks (no bare `stage_transfer(&DmaRegion<T>)`)
- the capability-gating contract (`absent` legal only in a concrete binding)
- DMA lowering family conditioned by `bus_type`; channel config derives from
  declaration + Layer 2 data
- streaming handle use within `DmaStreamBuffer` lifetime rules
- no DMA completion *delivery* inferred from backend-specific callback APIs this cycle

## Lowering Baseline

Lowering ownership:

- coherent allocation/release: compiler-generated
- channel request/release: compiler-generated
- the `store_order` fence: compiler-generated (dma-management owns it; register-access
  emits the plain doorbell)
- SG construction and address translation: compiler-internal (SG designed-not-built)
- `element_access`, A2 addressing, streaming map/acquire/release/unmap, and the channel
  ops: the backend lowers the stable surface (generic-c host DMA model this cycle;
  Linux/Zephyr designed-not-built)

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `alloc_coherent` | yes | yes | no | no |
| `request_channel` | yes | yes | no | no |
| `set_dma_mask` | yes | no | no | no |
| `sg_construction` | compiler-internal (designed-not-built) | compiler-internal | no | no |
| `address_translation` | compiler-internal | compiler-internal | no | no |

Accepted PM rule:

- DMA is class `V` under PM
- DMA quiesce/restart is driver-authored and compiler-validated
- DMA does not appear in the generated PM sequence

## Cross-Construct Dependencies

Depends on:

- `Bus Resource Management` for `bus_type`, `set_bus_master`, and bus-handle context
- `Register Access` for the doorbell/register programming that consumes DMA-visible
  addresses (register-access emits the plain doorbell; dma-management inserts the fence)
- `Device power state management` for PM interaction rules

Feeds:

- `Device interrupt setup and handling` as the future completion-delivery path (the
  registration shape pre-decides the callback-vs-raw-IRQ boundary)
- device-class constructs such as `Serial/UART Management` that use DMA as a transport
  path

## Constraints and Caveats

- Keep the L1 surface OS-agnostic (the C08 names); the Linux/Zephyr API names live in
  the lowering tables only.
- Preserve the `DmaStreamBuffer` handle model and RAII unmap behavior.
- Treat Linux dmaengine descriptor callbacks as lowering-internal; completion delivery
  is designed-not-built this cycle (registration shape only).
- Treat PL011 fallback-to-PIO as driver-authored control flow (device-class).
- The `store_order` fence always fires (ordering, not cache); cache maintenance is
  gated by L2 `region_cache_maintenance` (`per_access` requires cacheline isolation).

## Representative Examples

```rust
struct Pl011 {
    #[rseq::dma_channel(id = "rx", direction = "device_to_host")]
    rx_dma: DmaChannel,
    #[rseq::dma_region(id = "rx_buf", element_size = 1, element_count = 64)]
    rx_buf: DmaRegion<u8>,
}

fn start_rx_dma(&self) -> Result<(), RegseqError> {
    self.rx_dma.configure();
    self.rx_dma.stage_transfer(self.rx_buf.span(0..32))?;
    self.rx_dma.start_transfer()
}
```

## Implementation Checklist

- Recognize DMA region/channel/stream declarations + the completion-handler binding.
- Join DMA declarations with Layer 2 `device_address_bits` / `alignment` /
  `region_cache_maintenance` / `endpoint_addr_width` / `endpoint_max_burst_beats` /
  `binding_scope` / `cap_*` data.
- Implement the stable `DmaStreamBuffer` handle surface (map/acquire/release + RAII unmap).
- Lower `element_access`, the A2 ops, `span`, `configure`, `stage_transfer`,
  `start_transfer`, `stop`, `remaining_bytes`, and the `store_order` fence with the
  accepted OS-agnostic semantics.
- Enforce the C03 layout verify, the C10 fallibility lint, the anti-form/direction/span
  checks, and the capability-gating contract.
- Keep SG construction and address translation compiler-managed (SG designed-not-built).
- Keep DMA PM behavior in the compiler-validated / driver-authored bucket.
- Designed-not-built this cycle: completion delivery, `completion_marker_order`/`rmb`,
  the capability ops, Linux/Zephyr lowering + Tier-2 (see the deferrals section).
